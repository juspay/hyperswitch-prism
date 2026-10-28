pub mod transformers;

use std::fmt::Debug;

use super::macros;
use crate::types::ResponseRouterData;
use crate::with_error_response_body;
use common_enums::{AttemptStatus, CurrencyUnit, RefundStatus};
use common_utils::{consts, errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;
use domain_types::errors::WebhookError;
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, PSync, PostAuthenticate, PreAuthenticate,
        RSync, Refund, RepeatPayment, SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenRequestData, ConnectorWebhookSecrets, EventContext, EventType,
        PaymentFlowData, PaymentVoidData, PaymentWebhookReference, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsPostAuthenticateData, PaymentsPreAuthenticateData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData,
        RefundWebhookDetailsResponse, RefundWebhookReference, RefundsData, RefundsResponseData,
        RepeatPaymentData, RequestDetails, ResponseId, SetupMandateRequestData,
        WebhookDetailsResponse, WebhookResourceReference,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, WalletData},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{self, AuthenticationStep, RedirectState},
    decode::BodyDecoding,
    verification::SourceVerification,
};
use serde::Serialize;
use transformers as nexixpay;
use transformers::{
    NexixpayAuthorizeRequest, NexixpayAuthorizeResponse, NexixpayCaptureRequest,
    NexixpayCaptureResponse, NexixpayClientAuthRequest, NexixpayClientAuthResponse,
    NexixpayMandatePaymentRequest, NexixpayPostAuthenticateRequest,
    NexixpayPostAuthenticateResponse, NexixpayPreAuthenticateRequest,
    NexixpayPreAuthenticateResponse, NexixpayRSyncResponse, NexixpayRefundRequest,
    NexixpayRefundResponse, NexixpayRepeatPaymentResponse, NexixpaySetupMandateRequest,
    NexixpaySetupMandateResponse, NexixpaySyncResponse, NexixpayVoidRequest, NexixpayVoidResponse,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const X_API_KEY: &str = "X-Api-Key";
    pub(crate) const CORRELATION_ID: &str = "Correlation-Id";
    pub(crate) const IDEMPOTENCY_KEY: &str = "Idempotency-Key";
}

// ===== MACRO-BASED STRUCT AND BRIDGE SETUP =====
macros::create_all_prerequisites!(
    connector_name: Nexixpay,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: NexixpayAuthorizeRequest,
            response_body: NexixpayAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: NexixpaySyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: NexixpayVoidRequest,
            response_body: NexixpayVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: NexixpayCaptureRequest,
            response_body: NexixpayCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: NexixpayRefundRequest,
            response_body: NexixpayRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: NexixpayRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: PreAuthenticate,
            request_body: NexixpayPreAuthenticateRequest,
            response_body: NexixpayPreAuthenticateResponse,
            router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: PostAuthenticate,
            request_body: NexixpayPostAuthenticateRequest,
            response_body: NexixpayPostAuthenticateResponse,
            router_data: RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: ClientAuthenticationToken,
            request_body: NexixpayClientAuthRequest,
            response_body: NexixpayClientAuthResponse,
            router_data: RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: NexixpaySetupMandateRequest,
            response_body: NexixpaySetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: NexixpayMandatePaymentRequest,
            response_body: NexixpayRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            let mut auth_header = self.get_auth_header(&req.connector_config)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.nexixpay.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.nexixpay.base_url
        }

        pub fn connector_base_url_merchant_auth<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, MerchantAuthenticationFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.nexixpay.base_url
        }
    }
);

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATIONS =====
// Main service trait - aggregates all other traits

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Nexixpay<T>
{
}

// ===== PAYMENT FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Nexixpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Nexixpay<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Nexixpay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Nexixpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Nexixpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Nexixpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Nexixpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Nexixpay<T>
{
}

// ===== REFUND FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Nexixpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Nexixpay<T>
{
}

// ===== ADVANCED FLOW TRAIT IMPLEMENTATIONS =====
// ===== AUTHENTICATION FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Nexixpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Nexixpay<T>
{
}

// ===== DISPUTE FLOW TRAIT IMPLEMENTATIONS =====
// ===== WEBHOOK TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Nexixpay<T>
{
    /// Always reports the notification as unverified (`Ok(false)`), by design.
    ///
    /// Nexi signs nothing. The only authenticity proof XPay documents is that the body's
    /// `securityToken` matches the per-transaction token Nexi returned when the order was
    /// initialised through HPP / Build / PayByLink / googlepay ("Only if there is a match can
    /// the notification be considered valid", spec: Webhook Authentication). The S2S 3steps
    /// flow returns no such token, and this trait receives no per-transaction state, so there
    /// is nothing trustworthy to compare against. Comparing to a static configured secret is
    /// not an authenticity check: a forged body carrying that value would verify.
    ///
    /// Failing closed keeps the event parsed and mapped while Hyperswitch treats it as
    /// unverified and confirms the payment/refund state through PSync / RSync.
    fn verify_webhook_source(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, Report<WebhookError>> {
        Ok(false)
    }

    fn get_event_type(&self, request: RequestDetails) -> Result<EventType, Report<WebhookError>> {
        let notification = nexixpay::parse_nexixpay_webhook(&request.body)?;
        nexixpay::get_nexixpay_webhook_event_type(&notification.operation)
            .ok_or_else(|| WebhookError::WebhookEventTypeNotFound.into())
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, Report<WebhookError>> {
        let notification = nexixpay::parse_nexixpay_webhook(&request.body)?;
        let operation = notification.operation;
        Ok(
            match nexixpay::get_nexixpay_webhook_event_type(&operation) {
                // orderId is the connector_transaction_id of every Nexi payment flow.
                Some(event) if event.is_payment_event() => {
                    Some(WebhookResourceReference::Payment(PaymentWebhookReference {
                        connector_transaction_id: Some(operation.order_id),
                        merchant_transaction_id: None,
                        connector_preprocessing_id: None,
                    }))
                }
                // The refund's operationId is the connector_refund_id returned by Refund.
                Some(event) if event.is_refund_event() => {
                    Some(WebhookResourceReference::Refund(RefundWebhookReference {
                        connector_refund_id: Some(operation.operation_id),
                        merchant_refund_id: None,
                        connector_transaction_id: Some(operation.order_id),
                        merchant_transaction_id: None,
                    }))
                }
                Some(_) | None => None,
            },
        )
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, Report<WebhookError>> {
        let notification = nexixpay::parse_nexixpay_webhook(&request.body)?;
        let operation = notification.operation;
        match nexixpay::get_nexixpay_webhook_event_type(&operation) {
            Some(event) if event.is_payment_event() => {}
            Some(_) | None => return Err(WebhookError::WebhookEventTypeNotFound.into()),
        }
        // The notification's operation has the shape of GET /operations, so it maps onto the
        // attempt status as PSync does -- except for PENDING, which a notification must not
        // report as AuthenticationPending: see `get_webhook_attempt_status`.
        let status = nexixpay::get_webhook_attempt_status(&operation, AttemptStatus::Pending);
        let (error_code, error_message, error_reason) = if status == AttemptStatus::Failure {
            let result = operation.operation_result.to_string();
            (
                Some(consts::NO_ERROR_CODE.to_string()),
                Some(result.clone()),
                Some(result),
            )
        } else {
            (None, None, None)
        };
        Ok(WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(
                operation.order_id.clone(),
            )),
            status,
            connector_response_reference_id: Some(operation.order_id),
            connector_request_reference_id: None,
            mandate_reference: None,
            error_code,
            error_message,
            error_reason,
            raw_connector_response: None,
            status_code: 200,
            response_headers: None,
            amount_captured: None,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
            connector_returned_payment_method_details: None,
        })
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, Report<WebhookError>> {
        let notification = nexixpay::parse_nexixpay_webhook(&request.body)?;
        let operation = notification.operation;
        match nexixpay::get_nexixpay_webhook_event_type(&operation) {
            Some(event) if event.is_refund_event() => {}
            Some(_) | None => return Err(WebhookError::WebhookEventTypeNotFound.into()),
        }
        let status = nexixpay::get_nexixpay_webhook_refund_status(&operation.operation_result)
            .ok_or(WebhookError::WebhookEventTypeNotFound)?;
        let (error_code, error_message) = if status == RefundStatus::Failure {
            (
                Some(consts::NO_ERROR_CODE.to_string()),
                Some(operation.operation_result.to_string()),
            )
        } else {
            (None, None)
        };
        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: Some(operation.operation_id),
            merchant_transaction_id: None,
            status,
            connector_response_reference_id: Some(operation.order_id),
            error_code,
            error_message,
            raw_connector_response: None,
            status_code: 200,
            response_headers: None,
        })
    }

    /// The documented XPay notification example (spec: Webhook Payload Structure).
    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"eventId":"554ccc00-28fb-4344-a3fa-4bb8d1999bd5","eventTime":"2022-09-01T01:20:00.001Z","securityToken":"2f0ea5059b41414ca3744fe672327d85","operation":{"orderId":"btid2384983","operationId":"3470744","channel":"ECOMMERCE","operationType":"CAPTURE","operationResult":"EXECUTED","operationTime":"2022-09-01T01:20:00.001Z","paymentMethod":"CARD","paymentCircuit":"VISA","paymentInstrumentInfo":"***6152","paymentEndToEndId":"e723hedsdew","cancelledOperationId":"","operationAmount":"3545","operationCurrency":"EUR","additionalData":{"authorizationCode":"647189","cardCountry":"ITA","threeDS":"FULL_SECURE","schemaTID":"MCS01198U"}}}"#
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Nexixpay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Nexixpay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Nexixpay<T>
{
}

// ===== VALIDATION TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Nexixpay<T>
{
    /// Nexi card 3DS is two server-to-server legs around a browser ACS step:
    /// PreAuthenticate (`/orders/3steps/init`) -> [cardholder at the ACS] ->
    /// PostAuthenticate (`/orders/3steps/validation`) -> Authorize
    /// (`/orders/3steps/payment`, the only charging call). Nexi has no S2S Authenticate
    /// call, so that step is never returned.
    ///
    /// Termination: the PostAuthenticate-completed arm comes first and returns
    /// Authorize, so after PostAuthenticate the loop always ends; PreAuthenticate is
    /// returned only from (InitialRequest, None), so it runs at most once; a
    /// frictionless init (no threeDSAuthUrl, so no redirect and no break) falls through
    /// to Authorize.
    fn next_authentication_step(
        &self,
        auth_type: common_enums::AuthenticationType,
        payment_method: common_enums::PaymentMethod,
        redirect_state: RedirectState,
        completed_step: Option<AuthenticationStep>,
    ) -> AuthenticationStep {
        if auth_type != common_enums::AuthenticationType::ThreeDs
            || payment_method != common_enums::PaymentMethod::Card
        {
            return AuthenticationStep::Authorize;
        }
        match (redirect_state, completed_step) {
            (_, Some(AuthenticationStep::PostAuthenticate)) => AuthenticationStep::Authorize,
            (RedirectState::InitialRequest, None) => AuthenticationStep::PreAuthenticate,
            (RedirectState::InitialRequest, Some(AuthenticationStep::PreAuthenticate)) => {
                AuthenticationStep::Authorize
            }
            (
                RedirectState::RedirectWithParams | RedirectState::RedirectWithoutParams,
                None | Some(AuthenticationStep::PreAuthenticate),
            ) => AuthenticationStep::PostAuthenticate,
            _ => AuthenticationStep::Authorize,
        }
    }
}

// ===== CONNECTOR CUSTOMER TRAIT IMPLEMENTATIONS =====
// ===== MAIN CONNECTOR INTEGRATION IMPLEMENTATIONS =====
// Authorize Flow
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nexixpay,
    curl_request: Json(NexixpayAuthorizeRequest<T>),
    curl_response: NexixpayAuthorizeResponse,
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
            // One Authorize flow serves both Nexi charging endpoints, chosen by payment
            // method: Google Pay (PSP decryption) or the 3-step card payment.
            match &req.request.payment_method_data {
                PaymentMethodData::Wallet(WalletData::GooglePay(_)) => Ok(format!(
                    "{}/orders/googlepay",
                    self.connector_base_url_payments(req)
                )),
                _ => Ok(format!("{}/orders/3steps/payment", self.connector_base_url_payments(req))),
            }
        }
    }
);

// Payment Sync
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nexixpay,
    curl_response: NexixpaySyncResponse,
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
            // GET request - only auth headers needed
            self.get_auth_header(&req.connector_config)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // GET /operations/{operationId} addresses a Nexi operation, never the orderId:
            // psyncFlow in connector_feature_data selects the authorization, capture or
            // cancel operationId (Hyperswitch get_payment_id). Absent feature data or a
            // missing id fails closed; there is no connector_transaction_id fallback, since
            // /operations/{orderId} is a guaranteed 404.
            let operation_id = nexixpay::get_payment_id(
                req.resource_common_data
                    .connector_feature_data
                    .as_ref()
                    .map(|metadata| metadata.peek().clone()),
                None,
            )?;
            Ok(format!("{}/operations/{}", self.connector_base_url_payments(req), operation_id))
        }
    }
);

// Payment Void
// IMPORTANT: NexiXPay does NOT have a dedicated /cancels endpoint
// Instead, void is implemented via the /refunds endpoint with the full authorized amount
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nexixpay,
    curl_request: Json(NexixpayVoidRequest),
    curl_response: NexixpayVoidResponse,
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
            let mut header = self.build_headers(req)?;
            header.push((
                headers::IDEMPOTENCY_KEY.to_string(),
                common_utils::fp_utils::generate_uuid_v4().into(),
            ));
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Void is an online reversal: POST /operations/{operationId}/refunds on the
            // authorization operation, whose operationId only connector_feature_data carries.
            // connector_transaction_id is the Nexi orderId, which this endpoint cannot
            // address, so there is no fallback: absent feature data or a missing
            // authorizationOperationId fails closed before any HTTP call (Hyperswitch
            // MissingRequiredField parity).
            let metadata = req
                .resource_common_data
                .connector_feature_data
                .as_ref()
                .map(|metadata| metadata.peek().clone())
                .ok_or_else(|| IntegrationError::MissingRequiredField {
                    field_name: "connector_feature_data.authorizationOperationId",
                    context: nexixpay::nexi_ctx(
                        "Pass back the connector_feature_data returned by the Nexixpay Authorize call unchanged; it carries the authorization operationId.",
                        "Nexi voids address POST /operations/{authorizationOperationId}/refunds; the orderId in connector_transaction_id cannot be used.",
                    ),
                })?;
            let operation_id = nexixpay::get_payment_id(
                Some(metadata),
                Some(nexixpay::NexixpayPaymentIntent::Authorize),
            )?;
            Ok(format!("{}/operations/{}/refunds", self.connector_base_url_payments(req), operation_id))
        }
    }
);

// Payment Void Post Capture

// Payment Capture
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nexixpay,
    curl_request: Json(NexixpayCaptureRequest),
    curl_response: NexixpayCaptureResponse,
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
            let mut header = self.build_headers(req)?;
            header.push((
                headers::IDEMPOTENCY_KEY.to_string(),
                common_utils::fp_utils::generate_uuid_v4().into(),
            ));
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // POST /operations/{operationId}/captures confirms the authorization operation,
            // whose operationId only connector_feature_data carries. connector_transaction_id
            // is the Nexi orderId, which this endpoint cannot address, so there is no
            // fallback: absent feature data or a missing authorizationOperationId fails
            // closed before any HTTP call (Hyperswitch MissingRequiredField parity).
            let metadata = req
                .resource_common_data
                .connector_feature_data
                .as_ref()
                .map(|metadata| metadata.peek().clone())
                .ok_or_else(|| IntegrationError::MissingRequiredField {
                    field_name: "connector_feature_data.authorizationOperationId",
                    context: nexixpay::nexi_ctx(
                        "Pass back the connector_feature_data returned by the Nexixpay Authorize call unchanged; it carries the authorization operationId.",
                        "Nexi captures address POST /operations/{authorizationOperationId}/captures; the orderId in connector_transaction_id cannot be used.",
                    ),
                })?;
            let operation_id = nexixpay::get_payment_id(
                Some(metadata),
                Some(nexixpay::NexixpayPaymentIntent::Authorize),
            )?;
            Ok(format!("{}/operations/{}/captures", self.connector_base_url_payments(req), operation_id))
        }
    }
);

// Refund
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nexixpay,
    curl_request: Json(NexixpayRefundRequest),
    curl_response: NexixpayRefundResponse,
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
            let mut header = self.build_headers(req)?;
            header.push((
                headers::IDEMPOTENCY_KEY.to_string(),
                common_utils::fp_utils::generate_uuid_v4().into(),
            ));
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Refunds address POST /operations/{captureOperationId}/refunds. On explicit
            // terminals that is the Capture operation; on implicit (auto-captured) ones the
            // payment operation, which the Authorize response mapper records as
            // captureOperationId. connector_transaction_id is the Nexi orderId, which this
            // endpoint cannot address, so there is no fallback: absent feature data or a
            // missing captureOperationId fails closed before any HTTP call (Hyperswitch
            // MissingRequiredField parity).
            let metadata = req
                .request
                .connector_feature_data
                .as_ref()
                .map(|metadata| metadata.peek().clone())
                .ok_or_else(|| IntegrationError::MissingRequiredField {
                    field_name: "connector_feature_data.captureOperationId",
                    context: nexixpay::nexi_ctx(
                        "Pass back the connector_feature_data returned by the Nexixpay Authorize or Capture call unchanged; it carries the capture operationId.",
                        "Nexi refunds address POST /operations/{captureOperationId}/refunds; the orderId in connector_transaction_id cannot be used.",
                    ),
                })?;
            let operation_id = nexixpay::get_payment_id(
                Some(metadata),
                Some(nexixpay::NexixpayPaymentIntent::Capture),
            )?;
            Ok(format!("{}/operations/{}/refunds", self.connector_base_url_refunds(req), operation_id))
        }
    }
);

// Refund Sync
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nexixpay,
    curl_response: NexixpayRSyncResponse,
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
            // GET request - only auth headers needed
            self.get_auth_header(&req.connector_config)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // GET /operations/ with an empty id addresses a different resource, so an empty
            // connector_refund_id fails closed before any HTTP call (Hyperswitch
            // get_connector_refund_id parity).
            let connector_refund_id = &req.request.connector_refund_id;
            if connector_refund_id.trim().is_empty() {
                return Err(IntegrationError::MissingRequiredField {
                    field_name: "connector_refund_id",
                    context: nexixpay::nexi_ctx(
                        "Pass the connector_refund_id returned by the Nexixpay Refund call (the refund operationId).",
                        "Nexi refund sync addresses GET /operations/{operationId}; an empty operationId cannot identify the refund.",
                    ),
                }
                .into());
            }
            Ok(format!("{}/operations/{}", self.connector_base_url_refunds(req), connector_refund_id))
        }
    }
);

// Setup Mandate
// NexiXPay has no dedicated "tokenize the card without moving funds" endpoint, so
// SetupMandate reuses the `/orders/3steps/init` endpoint (same request shape as
// PreAuthenticate — see the `NexixpaySetupMandateRequest` type alias) with
// `recurrence.action = CONTRACT_CREATION` + `contractType = MIT_UNSCHEDULED` +
// `actionType = VERIFY` and `amount = 0`. The merchant-assigned `contractId`
// (derived from `connector_request_reference_id`) is surfaced back as
// `mandate_reference.connector_mandate_id` so a future RepeatPayment / MIT call
// can replay it — mirroring hyperswitch. The Nexi `operationId` is NOT the
// mandate anchor; it is kept on `connector_feature_data` / `preprocessing_id`
// only so the downstream PostAuthenticate / PSync flows can drive the 3DS
// handshake. Status comes back as `AuthenticationPending` (3DS challenge
// required) for a real card — the customer completes the 3DS hop on Nexi's
// hosted page.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nexixpay,
    curl_request: Json(NexixpaySetupMandateRequest<T>),
    curl_response: NexixpaySetupMandateResponse,
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
            Ok(format!("{}/orders/3steps/init", self.connector_base_url_payments(req)))
        }
    }
);

// Repeat Payment (MIT) — POST /orders/mit replaying the stored contractId
// (surfaced by SetupMandate as mandate_reference.connector_mandate_id). Single
// off-session call, no 3DS. Mirrors hyperswitch, which routes off-session
// Authorize payments to /orders/mit.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nexixpay,
    curl_request: Json(NexixpayMandatePaymentRequest<T>),
    curl_response: NexixpayRepeatPaymentResponse,
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
            Ok(format!("{}/orders/mit", self.connector_base_url_payments(req)))
        }
    }
);

// Sdk Session Token - ClientAuthenticationToken
// Uses the /orders/hpp endpoint to create a hosted payment page order
// Returns a securityToken and hostedPage URL for client-side SDK initialization
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nexixpay,
    curl_request: Json(NexixpayClientAuthRequest),
    curl_response: NexixpayClientAuthResponse,
    flow_name: ClientAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ClientAuthenticationTokenRequestData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/orders/hpp", self.connector_base_url_merchant_auth(req)))
        }
    }
);

// ===== AUTHENTICATION FLOW CONNECTOR INTEGRATIONS =====
// Pre Authentication (Step 1 - Initialize)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nexixpay,
    curl_request: Json(NexixpayPreAuthenticateRequest<T>),
    curl_response: NexixpayPreAuthenticateResponse,
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
            Ok(format!("{}/orders/3steps/init", self.connector_base_url_payments(req)))
        }
    }
);

// Authentication

// Post Authentication (Step 3 - Validation)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nexixpay,
    curl_request: Json(NexixpayPostAuthenticateRequest<T>),
    curl_response: NexixpayPostAuthenticateResponse,
    flow_name: PostAuthenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsPostAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/orders/3steps/validation", self.connector_base_url_payments(req)))
        }
    }
);

// ===== CONNECTOR CUSTOMER CONNECTOR INTEGRATIONS =====
// Create Connector Customer

// ===== SOURCE VERIFICATION IMPLEMENTATIONS =====

// ===== AUTHENTICATION FLOW SOURCE VERIFICATION =====

// ===== CONNECTOR CUSTOMER SOURCE VERIFICATION =====

// ===== CONNECTOR COMMON IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Nexixpay<T>
{
    fn id(&self) -> &'static str {
        "nexixpay"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.nexixpay.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = nexixpay::NexixpayAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: nexixpay::nexi_ctx(
                    "Send x-connector-config with a Nexixpay entry carrying api_key.",
                    "Nexi requires the X-Api-Key header on every call.",
                ),
            },
        )?;
        Ok(vec![
            (headers::X_API_KEY.to_string(), auth.api_key.into_masked()),
            (
                headers::CORRELATION_ID.to_string(),
                common_utils::fp_utils::generate_uuid_v4().into(),
            ),
        ])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        // Nexi answers 401 and 404 with an empty body; Hyperswitch fabricates canonical
        // errors for those, and treats any other empty body as NO_ERROR_CODE.
        let empty_body_error = |message: &str, reason: Option<String>| ErrorResponse {
            status_code: res.status_code,
            code: consts::NO_ERROR_CODE.to_string(),
            message: message.to_string(),
            reason,
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        };
        match res.status_code {
            401 => {
                return Ok(empty_body_error(
                    "UNAUTHORIZED",
                    Some("UNAUTHORIZED".to_string()),
                ))
            }
            404 => return Ok(empty_body_error("NOT FOUND", Some("NOT FOUND".to_string()))),
            _ if res.response.is_empty() => {
                return Ok(empty_body_error(consts::NO_ERROR_MESSAGE, None))
            }
            _ => {}
        }

        let response: nexixpay::NexixpayErrorResponse = res
            .response
            .parse_struct("NexixpayErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "nexixpay: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_error_response_body!(event_builder, response);

        // Extract the first error from the errors array
        let first_error = response.errors.first();

        // Concatenate all error descriptions for the reason field
        let concatenated_descriptions: Option<String> = {
            let descriptions: Vec<String> = response
                .errors
                .iter()
                .filter_map(|error| error.description.as_ref())
                .cloned()
                .collect();

            if descriptions.is_empty() {
                None
            } else {
                Some(descriptions.join(", "))
            }
        };

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: first_error
                .and_then(|error| error.code.clone())
                .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
            message: first_error
                .and_then(|error| error.description.clone())
                .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
            reason: concatenated_descriptions,
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

macros::macro_connector_flow_status_impls!(
    connector: Nexixpay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        IncrementalAuthorization,
        VoidPC,
        MandateRevoke,
        CreateOrder,
        ServerSessionAuthenticationToken,
        PaymentMethodToken,
        ServerAuthenticationToken,
        Authenticate,
    ],
    not_supported: [
        VoidPostRefund,
        Accept,
        DefendDispute,
        SubmitEvidence,
        CreateConnectorCustomer,
        GetConnectorCustomer,
    ],
);
