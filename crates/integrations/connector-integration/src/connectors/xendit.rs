pub mod transformers;

use std::fmt::Debug;

use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    crypto::{self, SignMessage, VerifySignature},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::FloatMajorUnit,
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, RepeatPayment, SetupMandate, Void},
    connector_types::{
        ConnectorWebhookSecrets, DisputeWebhookDetailsResponse, EventContext, EventType,
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData,
        RefundWebhookDetailsResponse, RefundsData, RefundsResponseData, RepeatPaymentData,
        RequestDetails, SetupMandateRequestData, WebhookDetailsResponse, WebhookResourceReference,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as xendit, XenditErrorResponse, XenditPayment as XenditCaptureResponse,
    XenditPayment as XenditVoidResponse, XenditPaymentRequestResponse,
    XenditPaymentRequestResponse as XenditPSyncResponse,
    XenditPaymentRequestResponse as XenditSetupMandateResponse,
    XenditPaymentRequestResponse as XenditRepeatPaymentResponse, XenditPaymentsCaptureRequest,
    XenditPaymentsRequest, XenditRefundRequest, XenditRefundResponse,
    XenditRefundResponse as RefundSyncResponse, XenditRepeatPaymentRequest,
    XenditSetupMandateRequest,
};

use super::macros;
use crate::{types::ResponseRouterData, utils, with_error_response_body};

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

use domain_types::errors::ConnectorError;
use domain_types::errors::{IntegrationError, IntegrationErrorContext, WebhookError};
use error_stack::ResultExt;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const API_VERSION: &str = "api-version";
}

/// Xendit v3 payment API version (spec "### API version used by the connector — decision").
pub(crate) const XENDIT_API_VERSION: &str = "2024-11-11";

macros::macro_connector_payout_implementation!(
    connector: Xendit,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Xendit<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Xendit<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Xendit<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Xendit<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Xendit<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Xendit<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Xendit<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Xendit<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Xendit<T>
{
}
macros::create_amount_converter_wrapper!(connector_name: Xendit, amount_type: FloatMajorUnit);
macros::create_all_prerequisites!(
    connector_name:  Xendit,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: XenditPaymentsRequest<T>,
            response_body: XenditPaymentRequestResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: XenditPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: XenditPaymentsCaptureRequest,
            response_body: XenditCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            response_body: XenditVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: XenditRefundRequest,
            response_body: XenditRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: RefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: XenditSetupMandateRequest<T>,
            response_body: XenditSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: XenditRepeatPaymentRequest<T>,
            response_body: XenditRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: FloatMajorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.get_content_type().to_string().into(),
            )];
            let mut api_key = self
                .get_auth_header(&req.connector_config)
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;
            header.append(&mut api_key);
            Ok(header)
        }

        /// Headers for every /v3/* call: build_headers + api-version. No idempotency-key.
        pub fn build_headers_v3<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let mut header = self.build_headers(req)?;
            header.push((
                headers::API_VERSION.to_string(),
                XENDIT_API_VERSION.to_string().into(),
            ));
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            // Trimmed so every path join yields a single slash.
            req.resource_common_data.connectors.xendit.base_url.trim_end_matches('/')
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            // Trimmed so every path join yields a single slash.
            req.resource_common_data.connectors.xendit.base_url.trim_end_matches('/')
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Xendit<T>
{
    fn id(&self) -> &'static str {
        "xendit"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = xendit::XenditAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    additional_context: Some("Xendit requires API key authentication".to_owned()),
                    ..Default::default()
                },
            },
        )?;
        let encoded_api_key = BASE64_ENGINE.encode(format!("{}:", auth.api_key.peek()));

        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Basic {encoded_api_key}").into_masked(),
        )])
    }

    fn base_url<'a>(&self, _connectors: &'a Connectors) -> &'a str {
        ""
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: XenditErrorResponse = res
            .response
            .parse_struct("XenditErrorResponse")
            .change_context(
                utils::response_deserialization_fail(
                    res.status_code,
                "xendit: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .error_code
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response
                .message
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.message.clone(),
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
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
    connector: Xendit,
    curl_request: Json(XenditPaymentsRequest),
    curl_response: XenditPaymentRequestResponse,
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
            self.build_headers_v3(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v3/payment_requests", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Xendit,
    curl_response: XenditPSyncResponse,
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
            self.build_headers_v3(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // the pr- payment request id is the path parameter.
            let payment_request_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Pass the connector_transaction_id (pr- payment request id) returned by the Xendit Authorize response".to_owned(),
                        ),
                        doc_url: Some("https://docs.xendit.co/apidocs/get-payment-request".to_owned()),
                        additional_context: Some(
                            "Xendit PSync reads GET /v3/payment_requests/{payment_request_id}".to_owned(),
                        ),
                    },
                })?;

            Ok(format!(
                "{}/v3/payment_requests/{payment_request_id}",
                self.connector_base_url_payments(req),
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Xendit,
    curl_request: Json(XenditPaymentsCaptureRequest),
    curl_response: XenditCaptureResponse,
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
            self.build_headers_v3(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // v3 capture addresses the py- payment id carried in
            // connector_feature_data; the pr- connector_transaction_id is never substituted.
            let payment_id =
                xendit::get_xendit_payment_id(req.request.connector_feature_data.as_ref())?;
            Ok(format!(
                "{}/v3/payments/{payment_id}/capture",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

// Void sends no body: the cancel-payment API documents no request schema
// (spec "##### Request", body EMPTY), so no curl_request (precedent: billwerk.rs Void).
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Xendit,
    curl_response: XenditVoidResponse,
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
            self.build_headers_v3(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Void is "Cancel a payment" on the py- payment id from
            // connector_feature_data. Never the pr- id, and never
            // /v3/payment_requests/{id}/cancel (that cancels the request, not the hold).
            let payment_id =
                xendit::get_xendit_payment_id(req.request.connector_feature_data.as_ref())?;
            Ok(format!(
                "{}/v3/payments/{payment_id}/cancel",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Xendit,
    curl_request: Json(XenditRefundRequest),
    curl_response: XenditRefundResponse,
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
                "{}/refunds",
                self.connector_base_url_refunds(req)
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Xendit,
    curl_response: RefundSyncResponse,
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
            // the rfd- refund id is the path parameter; an empty id would hit the list route.
            let connector_refund_id = &req.request.connector_refund_id;
            if connector_refund_id.trim().is_empty() {
                return Err(IntegrationError::MissingConnectorRefundID {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Pass the connector_refund_id (rfd- refund id) returned by the Xendit Refund response".to_owned(),
                        ),
                        doc_url: Some("https://docs.xendit.co/apidocs/refund-payment-request".to_owned()),
                        additional_context: Some(
                            "Xendit RSync reads GET /refunds/{refund_id}".to_owned(),
                        ),
                    },
                }
                .into());
            }
            Ok(format!(
                "{}/refunds/{connector_refund_id}",
                self.connector_base_url_refunds(req),
            ))
        }
    }
);

// SetupMandate = zero-amount card verification (spec "#### 9. Verify a payment method"):
// POST /v3/payment_requests with type VERIFY_PAYMENT_METHOD.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Xendit,
    curl_request: Json(XenditSetupMandateRequest),
    curl_response: XenditSetupMandateResponse,
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
            self.build_headers_v3(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v3/payment_requests", self.connector_base_url_payments(req)))
        }
    }
);

// RepeatPayment = merchant-initiated charge (spec "### RepeatPayment / Merchant-initiated
// transaction (MIT)"): POST /v3/payment_requests, Model A (pt- token) or Model B (PAN + NTID).
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Xendit,
    curl_request: Json(XenditRepeatPaymentRequest),
    curl_response: XenditRepeatPaymentResponse,
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
            self.build_headers_v3(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v3/payment_requests", self.connector_base_url_payments(req)))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Xendit<T>
{
}

// Incoming webhooks (spec "## Webhook Events"). ParseEvent: get_event_type +
// get_webhook_event_reference (stateless). HandleEvent: verify_webhook_source + process_*.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Xendit<T>
{
    /// Xendit signs nothing: every webhook carries the account's static callback token in
    /// `x-callback-token`, and that token authenticates the whole body (spec "### Webhook
    /// Authentication & Signature Verification"). The token is compared with the configured
    /// webhook secret in constant time: both sides are HMAC-SHA256'd under the secret and the
    /// two MACs are compared by ring's constant-time `hmac::verify`, so no byte-wise `==` ever
    /// runs on the secret. Missing or mismatched header -> Ok(false); no secret -> Err.
    /// The API key is never used as a fallback, and neither token is ever logged.
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        let configured_token = connector_webhook_secret
            .map(|secrets| secrets.secret)
            .filter(|secret| !secret.is_empty())
            .ok_or_else(|| error_stack::report!(WebhookError::WebhookVerificationSecretNotFound))?;

        let Some(received_token) = xendit::get_xendit_callback_token(&request.headers) else {
            return Ok(false);
        };

        let received_mac = crypto::HmacSha256
            .sign_message(&configured_token, received_token.as_bytes())
            .change_context(WebhookError::WebhookSourceVerificationFailed)?;
        crypto::HmacSha256
            .verify_signature(&configured_token, &received_mac, &configured_token)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        // Payment / refund envelopes and the flat dispute body all carry a top-level `event`.
        let envelope = xendit::parse_xendit_webhook_envelope(&request.body)?;
        xendit::get_xendit_webhook_event_type(envelope.event)
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        xendit::get_xendit_webhook_reference(&request.body)
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        xendit::build_xendit_payment_webhook_response(&request.body)
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        xendit::build_xendit_refund_webhook_response(&request.body)
    }

    fn process_dispute_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<DisputeWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        xendit::build_xendit_dispute_webhook_response(&request.body)
    }

    /// Spec "### Webhook Payload Structure" (`payment.authorization`), field-probe input.
    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"created":"2024-12-18T05:46:35.109Z","business_id":"62440e322008e87fb29c1fd0","event":"payment.authorization","data":{"type":"PAY","status":"AUTHORIZED","country":"ID","created":"2024-12-18T05:46:08.192Z","updated":"2024-12-18T05:46:30.627Z","currency":"IDR","payment_id":"py-3f57d678-2448-4c9f-a433-8468d366fb5c","business_id":"62440e322008e87fb29c1fd0","customer_id":"cust-7de9a9b4-37e8-40ad-b665-d97f42e538c5","channel_code":"CARDS","reference_id":"97ba0a32-b996-4abf-8a7b-6184a6644676_b8d18f2f-3","capture_method":"MANUAL","request_amount":10000,"payment_details":{"authorization_data":{"reconciliation_id":"7345007929096981703954","authorization_code":"831000","acquirer_merchant_id":"xendit_ctv_agg","network_response_code":"00","network_transaction_id":"016153570198200","cvn_verification_result":"M","retrieval_reference_number":"435205253972","address_verification_result":"M","network_response_code_descriptor":"Approved and completed sucessfully"},"authentication_data":{"flow":"CHALLENGE","a_res":{"eci":"05","message_version":"2.1.0","authentication_value":"AAIBBYNoEwAAACcKhAJkdQAAAAA=","directory_server_trans_id":"e537f539-d59f-4ebe-8d56-7fdc31a8e9b4"}}},"payment_request_id":"pr-5593127f-8c7b-4d2f-b487-c785ffc21e2f"},"api_version":"v3"}"#
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Xendit<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Xendit<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Xendit<T>
{
}

macros::macro_connector_flow_status_impls!(
    connector: Xendit,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        CreateOrder,
        ServerSessionAuthenticationToken,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        PaymentMethodToken,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        ClientAuthenticationToken,
        MandateRevoke,
    ],
    not_supported: [
        VoidPostRefund,
        IncrementalAuthorization,
        SubmitEvidence,
        DefendDispute,
        Accept,
        ServerAuthenticationToken,
        VoidPC,
    ],
);
