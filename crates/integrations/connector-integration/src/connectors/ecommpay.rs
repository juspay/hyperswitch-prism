//! Ecommpay — Gate API v2 (card, Apple Pay and Google Pay payments).
//!
//! Every Gate call is a JSON `POST` signed in the body (`general.signature`, HMAC-SHA512 with
//! the project's secret key); there is no authentication header. Payment-processing calls are
//! asynchronous: HTTP 200 means the request was accepted, and the result is read with
//! `POST /v2/payment/status` or arrives by callback.
//!
//! 3-D Secure has no authentication-only endpoint: it runs inside the `sale`/`auth` payment.
//! A 3DS card payment is therefore three caller RPCs, none of them a PreAuthenticate /
//! Authenticate / PostAuthenticate leg:
//!
//! * **Authorize** — `POST /v2/payment/card/sale` (automatic capture) or
//!   `/v2/payment/card/auth` (manual capture). This is the charging call.
//! * **PSync** — `POST /v2/payment/status`, which returns the ACS redirect while the payment
//!   is `awaiting 3ds result`.
//! * **Authorize** again, with `redirect_response` — `POST /v2/payment/card/3ds_result`, which
//!   completes the suspended `sale`/`auth` (same `general.payment_id`, no new operation).
//!   Selected by [`transformers::is_three_ds_completion`], which the body builder uses too.
//!
//! Callbacks (`IncomingWebhook`) are verified with the merchant's webhook secret over the same
//! signature preimage as requests; see the impl below.
//!
//! Reference: `grace/rulesbook/codegen/references/ecommpay/technical_specification.md`

pub mod transformers;

use std::fmt::Debug;

use base64::Engine;
use common_enums::{AttemptStatus, CaptureMethod, CurrencyUnit};
use common_utils::{
    consts::BASE64_ENGINE,
    crypto::{self, VerifySignature},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::MinorUnit,
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, RepeatPayment, SetupMandate, Void},
    connector_types::*,
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding,
};
use serde::Serialize;
use transformers::{
    self as ecommpay, EcommpayAcceptanceResponse, EcommpayAccountVerificationRequest,
    EcommpayAuthorizeRequest, EcommpayCaptureRequest, EcommpayCaptureResponse,
    EcommpayRawStatusResponse, EcommpayRefundRequest, EcommpayRefundResponse,
    EcommpayRefundSyncRequest, EcommpayRefundSyncResponse, EcommpayRepeatPaymentRequest,
    EcommpayRepeatPaymentResponse, EcommpaySetupMandateResponse, EcommpayStatusRequest,
    EcommpayVoidRequest, EcommpayVoidResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

const ECOMMPAY_JSON_MEDIA_TYPE: &str = "application/json";

/// Last path segment of a one-step payment: `/v2/payment/{card|applepay|googlepay}/sale`.
const SALE_OR_AUTH_SALE: &str = "sale";
/// Last path segment of a two-step payment's authorisation: `/v2/payment/{route}/auth`.
const SALE_OR_AUTH_AUTH: &str = "auth";
const CARD_THREE_DS_RESULT_ENDPOINT: &str = "/v2/payment/card/3ds_result";
/// Synchronous payment-status request (PSync and RSync); the payment is addressed by
/// `general.payment_id`.
const PAYMENT_STATUS_ENDPOINT: &str = "/v2/payment/status";
/// Last path segment of a two-step payment's capture: `/v2/payment/{route}/capture`.
const CAPTURE_OPERATION: &str = "capture";
/// Last path segment of a hold release: `/v2/payment/{route}/cancel`.
const CANCEL_OPERATION: &str = "cancel";
/// Last path segment of a refund: `/v2/payment/{route}/refund`.
const REFUND_OPERATION: &str = "refund";
/// Zero-amount card verification with COF registration (SetupMandate).
const CARD_ACCOUNT_VERIFICATION_ENDPOINT: &str = "/v2/payment/card/account_verification";

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Ecommpay<T>
{
    fn id(&self) -> &'static str {
        "ecommpay"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        ECOMMPAY_JSON_MEDIA_TYPE
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.ecommpay.base_url.as_ref()
    }

    /// No authentication header exists: the request is authenticated by the body signature.
    /// The credentials are still validated here so a malformed config fails before any call.
    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        ecommpay::EcommpayAuthType::try_from(auth_type)?;
        Ok(Vec::new())
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        match res
            .response
            .parse_struct::<ecommpay::EcommpayErrorResponse>("EcommpayErrorResponse")
        {
            Ok(response) => {
                with_error_response_body!(event_builder, response);
                let details = response.details();
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: details.code,
                    message: details.message,
                    reason: details.reason,
                    attempt_status: None,
                    // A 4xx Gate error body carries no payment identifier.
                    connector_transaction_id: None,
                    ..Default::default()
                })
            }
            Err(_) => {
                let raw_body = String::from_utf8_lossy(&res.response).to_string();
                tracing::warn!(
                    status_code = res.status_code,
                    "Ecommpay returned an error body that is neither ErrorResponse nor ErrorItems"
                );
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: common_utils::consts::NO_ERROR_CODE.to_string(),
                    message: common_utils::consts::NO_ERROR_MESSAGE.to_string(),
                    reason: Some(raw_body.chars().take(500).collect()),
                    attempt_status: None,
                    connector_transaction_id: None,
                    ..Default::default()
                })
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Ecommpay<T>
{
}

macros::create_amount_converter_wrapper!(connector_name: Ecommpay, amount_type: MinorUnit);

macros::create_all_prerequisites!(
    connector_name: Ecommpay,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: EcommpayAuthorizeRequest<T>,
            response_body: EcommpayAcceptanceResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: EcommpayStatusRequest,
            response_body: EcommpayRawStatusResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: EcommpayCaptureRequest,
            response_body: EcommpayCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: EcommpayVoidRequest,
            response_body: EcommpayVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: EcommpayRefundRequest,
            response_body: EcommpayRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: EcommpayRefundSyncRequest,
            response_body: EcommpayRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: EcommpayAccountVerificationRequest<T>,
            response_body: EcommpaySetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: EcommpayRepeatPaymentRequest<T>,
            response_body: EcommpayRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: MinorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                self.get_content_type().to_string().into(),
            )])
        }

        /// The configured base URL without its trailing `/`, so `{base}{/v2/...}` emits one
        /// slash. A per-merchant `base_url` in the Ecommpay config takes precedence.
        ///
        /// The two wrappers below exist only because the flow-data type differs between the
        /// payment and refund flows and nothing both implement exposes `connectors`.
        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            ecommpay_base_url(
                &req.connector_config,
                &req.resource_common_data.connectors,
            )
        }

        /// [`Self::connector_base_url_payments`] for the refund flows.
        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            ecommpay_base_url(
                &req.connector_config,
                &req.resource_common_data.connectors,
            )
        }
    }
);

/// The Ecommpay base URL to call, with any trailing `/` removed: a per-merchant `base_url` in
/// the connector config wins over the environment default.
fn ecommpay_base_url<'a>(
    connector_config: &'a ConnectorSpecificConfig,
    connectors: &'a Connectors,
) -> &'a str {
    let configured = match connector_config {
        ConnectorSpecificConfig::Ecommpay {
            base_url: Some(base_url),
            ..
        } => base_url.as_str(),
        _ => connectors.ecommpay.base_url.as_str(),
    };
    configured.trim_end_matches('/')
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Ecommpay,
    curl_request: Json(EcommpayAuthorizeRequest),
    curl_response: EcommpayAcceptanceResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            // The 3DS completion resumes the same sale/auth, whatever its capture method.
            if ecommpay::is_three_ds_completion(&req.request) {
                return Ok(format!(
                    "{}{}",
                    self.connector_base_url_payments(req),
                    CARD_THREE_DS_RESULT_ENDPOINT
                ));
            }
            // Apple Pay and Google Pay (PSP- or Hyperswitch-decrypted) use their own path family;
            // a card uses the card path.
            let route = ecommpay::authorize_route(&req.request.payment_method_data);
            let operation = match req.request.capture_method {
                Some(CaptureMethod::Manual) => SALE_OR_AUTH_AUTH,
                Some(CaptureMethod::Automatic | CaptureMethod::SequentialAutomatic) | None => {
                    SALE_OR_AUTH_SALE
                }
                    // Refused while the body is built; never reaches the wire.
                    Some(CaptureMethod::ManualMultiple | CaptureMethod::Scheduled) => {
                        return Err(errors::IntegrationError::CaptureMethodNotSupported {
                            context: errors::IntegrationErrorContext {
                                suggested_action: Some(
                                    "Use capture_method AUTOMATIC or MANUAL.".to_string(),
                                ),
                                doc_url: Some(
                                    "https://developers.ecommpay.com/en/en_gate_payment_capture.html"
                                        .to_string(),
                                ),
                                additional_context: Some(
                                    "Ecommpay Gate offers sale (one-step) and auth + capture \
                                     (two-step) only"
                                        .to_string(),
                                ),
                            },
                        }
                        .into());
                    }
            };
            Ok(format!(
                "{}/v2/payment/{}/{}",
                self.connector_base_url_payments(req),
                route.path_segment(),
                operation
            ))
        }

        /// A 400/403/422 means the Gate rejected the request and created no operation, so the
        /// attempt failed; any other status leaves the outcome open. 5xx responses go through
        /// `get_5xx_error_response`, which leaves `attempt_status` unset.
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
            let status_code = res.status_code;
            let mut error = self.build_error_response(res, event_builder, connector_config)?;
            error.attempt_status = match status_code {
                400 | 403 | 422 => Some(FlowStatus::Payment(AttemptStatus::Failure)),
                _ => None,
            };
            Ok(error)
        }
    }
);

// PSync: `POST /v2/payment/status`. The response is kept as raw JSON until its `signature` is
// verified; a 4xx/5xx leaves `attempt_status` unset, so the router keeps the
// status it already had.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Ecommpay,
    curl_request: Json(EcommpayStatusRequest),
    curl_response: EcommpayRawStatusResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                PAYMENT_STATUS_ENDPOINT
            ))
        }
    }
);

// Capture: `POST /v2/payment/{card|applepay|googlepay}/capture` on the path family the payment
// was authorised on (recorded by Authorize in `connector_metadata`; card when absent). The
// response is an acceptance only — the captured amount is confirmed by PSync or callback.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Ecommpay,
    curl_request: Json(EcommpayCaptureRequest),
    curl_response: EcommpayCaptureResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let route =
                ecommpay::route_from_feature_data(req.request.connector_feature_data.as_ref());
            Ok(format!(
                "{}/v2/payment/{}/{}",
                self.connector_base_url_payments(req),
                route.path_segment(),
                CAPTURE_OPERATION
            ))
        }

        /// A 400/403/422 means the Gate rejected the capture and created no operation, so the
        /// capture failed (the hold is untouched); any other status leaves the outcome open.
        /// 5xx responses go through `get_5xx_error_response`, which leaves `attempt_status` unset.
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
            let status_code = res.status_code;
            let mut error = self.build_error_response(res, event_builder, connector_config)?;
            error.attempt_status = match status_code {
                400 | 403 | 422 => Some(FlowStatus::Payment(AttemptStatus::CaptureFailed)),
                _ => None,
            };
            Ok(error)
        }
    }
);

// Void: `POST /v2/payment/{card|applepay|googlepay}/cancel` on the path family the payment was
// authorised on (card when Authorize recorded none). Full release only; the response is an
// acceptance, and `canceled` is read by PSync or delivered by callback.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Ecommpay,
    curl_request: Json(EcommpayVoidRequest),
    curl_response: EcommpayVoidResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let route =
                ecommpay::route_from_feature_data(req.request.connector_feature_data.as_ref());
            Ok(format!(
                "{}/v2/payment/{}/{}",
                self.connector_base_url_payments(req),
                route.path_segment(),
                CANCEL_OPERATION
            ))
        }

        /// A 4xx other than 408/429 means the Gate rejected the cancel and created no
        /// operation, so the void failed and the hold is untouched. 5xx responses go through
        /// `get_5xx_error_response`, which leaves `attempt_status` unset. An HTTP 200 is never
        /// a failure here: a cancel declined after acceptance is reported by PSync, not by this
        /// response.
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
            let status_code = res.status_code;
            let mut error = self.build_error_response(res, event_builder, connector_config)?;
            error.attempt_status = match status_code {
                // 408 and 429 are transient: a timeout may still have created the operation
                // and a rate limit created nothing but is retryable, so neither is written off
                // here -- the outcome stays open for PSync.
                408 | 429 => None,
                400..=499 => Some(FlowStatus::Payment(AttemptStatus::VoidFailed)),
                _ => None,
            };
            Ok(error)
        }
    }
);

// Refund: `POST /v2/payment/{card|applepay|googlepay}/refund` on the path family the payment was
// created on (card when Authorize recorded none), addressed by the original purchase's
// `general.payment_id`. The response is an acceptance; the refund result is read by RSync or
// delivered by callback.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Ecommpay,
    curl_request: Json(EcommpayRefundRequest),
    curl_response: EcommpayRefundResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let route =
                ecommpay::route_from_feature_data(req.request.connector_feature_data.as_ref());
            Ok(format!(
                "{}/v2/payment/{}/{}",
                self.connector_base_url_refunds(req),
                route.path_segment(),
                REFUND_OPERATION
            ))
        }

        /// A refund rejected by the Gate (e.g. `3283`, `3285` two-minute rule, `3230` duplicate
        /// `merchant_refund_id`) keeps its code and message, and carries no payment
        /// `attempt_status`: a failed refund must never move the payment's status. The
        /// refund status is set by the router from the error itself.
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
            let mut error = self.build_error_response(res, event_builder, connector_config)?;
            error.attempt_status = None;
            Ok(error)
        }
    }
);

// RSync: there is no refund-status endpoint. The refund is read from `POST /v2/payment/status`
// of the original purchase (`general.payment_id`), as the `refund`/`reversal` operation whose
// `request_id` is the connector refund id. The response is kept as raw JSON until its
// `signature` is verified. A 4xx/5xx carries no status (default
// `get_error_response_v2`), so a failed sync never terminally fails the refund.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Ecommpay,
    curl_request: Json(EcommpayRefundSyncRequest),
    curl_response: EcommpayRefundSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_refunds(req),
                PAYMENT_STATUS_ENDPOINT
            ))
        }
    }
);

// SetupMandate: `POST /v2/payment/card/account_verification` — a zero-amount card verification
// that registers the card for COF purchases (`recurring.register`, platform storage). The
// response is an acceptance only: the verification result and `recurring.id` (the mandate id)
// are read by PSync or delivered by callback, so this flow never reports a terminal status.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Ecommpay,
    curl_request: Json(EcommpayAccountVerificationRequest),
    curl_response: EcommpaySetupMandateResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                CARD_ACCOUNT_VERIFICATION_ENDPOINT
            ))
        }

        /// A 4xx other than 408/429 means the Gate rejected the verification and created no
        /// operation, so the setup failed. 5xx responses go through `get_5xx_error_response`,
        /// which leaves `attempt_status` unset, so the outcome stays open.
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
            let status_code = res.status_code;
            let mut error = self.build_error_response(res, event_builder, connector_config)?;
            error.attempt_status = match status_code {
                // 408 and 429 are transient: a timeout may still have created the operation
                // and a rate limit created nothing but is retryable, so neither is written off
                // here -- the outcome stays open for PSync.
                408 | 429 => None,
                400..=499 => Some(FlowStatus::Payment(AttemptStatus::Failure)),
                _ => None,
            };
            Ok(error)
        }
    }
);

// RepeatPayment (MIT / COF debit): `POST /v2/payment/card/recurring` by `recurring.id` (platform
// storage), or `POST /v2/payment/card/{sale|auth}` with the stored card, `stored_card_type` and
// `scheme_id` (merchant storage, NTID). The call is chosen by
// [`transformers::repeat_payment_operation`], which the body builder uses too. The response is
// an acceptance only: the debit result is read by PSync or delivered by callback.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Ecommpay,
    curl_request: Json(EcommpayRepeatPaymentRequest),
    curl_response: EcommpayRepeatPaymentResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let operation = ecommpay::repeat_payment_operation(&req.request)?;
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                operation.path()
            ))
        }

        /// A 4xx other than 408/429 means the Gate rejected the debit and created no
        /// operation, so the attempt failed; the code and message (e.g. `3363` missing trace
        /// id, `3184`/`3301`/`3183` cancelled or expired COF) are kept verbatim. 5xx responses
        /// go through `get_5xx_error_response`, which leaves `attempt_status` unset.
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
            let status_code = res.status_code;
            let mut error = self.build_error_response(res, event_builder, connector_config)?;
            error.attempt_status = match status_code {
                // 408 and 429 are transient: a timeout may still have created the operation
                // and a rate limit created nothing but is retryable, so neither is written off
                // here -- the outcome stays open for PSync.
                408 | 429 => None,
                400..=499 => Some(FlowStatus::Payment(AttemptStatus::Failure)),
                _ => None,
            };
            Ok(error)
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Ecommpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Ecommpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Ecommpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Ecommpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Ecommpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Ecommpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Ecommpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Ecommpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Ecommpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Ecommpay<T>
{
}

/// Payment/operation callbacks are signed in the body (top-level `signature`, the same
/// algorithm as requests); chargeback callbacks are unsigned by design and token callbacks sign
/// under `general.signature`, so neither is ever source-verified.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Ecommpay<T>
{
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<errors::WebhookError>> {
        // Only the configured webhook secret verifies a callback; the API
        // secret_key from the connector config is never substituted for it.
        let secrets = connector_webhook_secret
            .filter(|secrets| !secrets.secret.is_empty())
            .ok_or_else(|| {
                error_stack::report!(errors::WebhookError::WebhookVerificationSecretNotFound)
                    .attach_printable("Ecommpay callback verification needs a webhook secret")
            })?;
        match ecommpay::parse_webhook_body(&request.body)? {
            // Chargeback callbacks carry no signature and are never authoritative.
            ecommpay::EcommpayWebhookBody::Chargeback(_) => return Ok(false),
            ecommpay::EcommpayWebhookBody::Token(_) | ecommpay::EcommpayWebhookBody::Payment(_) => {
            }
        }
        let body: serde_json::Value = serde_json::from_slice(&request.body)
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;
        let Some(received) = body
            .get("signature")
            .and_then(serde_json::Value::as_str)
            .and_then(|signature| BASE64_ENGINE.decode(signature).ok())
        else {
            return Ok(false);
        };
        let preimage = ecommpay::ecommpay_canonical_string(&body);
        // Constant-time comparison; a computation error is a non-match, never a pass.
        match crypto::HmacSha512.verify_signature(&secrets.secret, &received, preimage.as_bytes()) {
            Ok(verified) => Ok(verified),
            Err(_) => {
                tracing::warn!(
                    "Ecommpay: callback signature could not be computed; treated as not verified"
                );
                Ok(false)
            }
        }
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
        let body = ecommpay::parse_webhook_body(&request.body)?;
        Ok(ecommpay::webhook_event_type(&body))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<errors::WebhookError>> {
        let body = ecommpay::parse_webhook_body(&request.body)?;
        ecommpay::webhook_reference(&body)
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<errors::WebhookError>> {
        match ecommpay::parse_webhook_body(&request.body)? {
            ecommpay::EcommpayWebhookBody::Payment(response) => {
                ecommpay::payment_webhook_details(&response)
            }
            ecommpay::EcommpayWebhookBody::Chargeback(_)
            | ecommpay::EcommpayWebhookBody::Token(_) => Err(error_stack::report!(
                errors::WebhookError::WebhookResourceObjectNotFound
            )
            .attach_printable("Ecommpay callback is not a payment callback")),
        }
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<errors::WebhookError>> {
        match ecommpay::parse_webhook_body(&request.body)? {
            ecommpay::EcommpayWebhookBody::Payment(response) => {
                ecommpay::refund_webhook_details(&response)
            }
            ecommpay::EcommpayWebhookBody::Chargeback(_)
            | ecommpay::EcommpayWebhookBody::Token(_) => Err(error_stack::report!(
                errors::WebhookError::WebhookResourceObjectNotFound
            )
            .attach_printable("Ecommpay callback is not a refund callback")),
        }
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"project_id":1,"payment":{"id":"probe_payment","status":"success"},"operation":{"id":1,"type":"sale","status":"success","request_id":"probe_request"},"signature":""}"#
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Ecommpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for Ecommpay<T>
{
}

crate::connectors::macros::macro_connector_payout_implementation!(
    connector: Ecommpay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: Ecommpay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept,
        ClientAuthenticationToken,
        CreateConnectorCustomer,
        DefendDispute,
        GetConnectorCustomer,
        MandateRevoke,
        Authenticate,
        IncrementalAuthorization,
        CreateOrder,
        PostAuthenticate,
        PreAuthenticate,
        PaymentMethodToken,
        VoidPC,
        VoidPostRefund,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        SubmitEvidence
    ],
);
