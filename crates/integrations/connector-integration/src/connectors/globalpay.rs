pub mod transformers;

use std::fmt::Debug;

use common_enums::{AttemptStatus, CurrencyUnit, RefundStatus};
use common_utils::{
    errors::CustomResult, events, ext_traits::ByteSliceExt, types::StringMinorUnit,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, PSync, PaymentMethodToken, PostAuthenticate,
        RSync, Refund, RepeatPayment, ServerAuthenticationToken, SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenRequestData, ConnectorWebhookSecrets, EventContext, EventType,
        PaymentFlowData, PaymentMethodTokenResponse, PaymentMethodTokenizationData,
        PaymentVoidData, PaymentWebhookReference, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsPostAuthenticateData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundWebhookDetailsResponse, RefundWebhookReference, RefundsData,
        RefundsResponseData, RepeatPaymentData, RequestDetails, ResponseId,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        SetupMandateRequestData, WebhookDetailsResponse, WebhookResourceReference,
    },
    errors::WebhookError,
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Mask, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use sha2::{Digest, Sha512};
use transformers as globalpay;
use transformers::{
    GlobalpayAccessTokenErrorResponse, GlobalpayAccessTokenRequest, GlobalpayAccessTokenResponse,
    GlobalpayAuthorizeResponse, GlobalpayCaptureRequest, GlobalpayCaptureResponse,
    GlobalpayClientAuthRequest, GlobalpayClientAuthResponse, GlobalpayConfirmRequest,
    GlobalpayConfirmResponse, GlobalpayPSyncResponse, GlobalpayPaymentMethodTokenRequest,
    GlobalpayPaymentMethodTokenResponse, GlobalpayPaymentsRequest, GlobalpayRSyncResponse,
    GlobalpayRefundRequest, GlobalpayRefundResponse, GlobalpayRepeatPaymentRequest,
    GlobalpayRepeatPaymentResponse, GlobalpaySetupMandateRequest, GlobalpaySetupMandateResponse,
    GlobalpayVoidRequest, GlobalpayVoidResponse,
};

use crate::connectors::macros;
use crate::types::ResponseRouterData;
use crate::with_error_response_body;
use domain_types::errors::{ConnectorError, IntegrationError, IntegrationErrorContext};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const X_GP_VERSION: &str = "X-GP-Version";
}

const API_VERSION: &str = "2021-03-22";

macros::create_amount_converter_wrapper!(connector_name: Globalpay, amount_type: StringMinorUnit);
macros::create_all_prerequisites!(
    connector_name: Globalpay,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: GlobalpayPaymentsRequest<T>,
            response_body: GlobalpayAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: GlobalpayPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: GlobalpayVoidRequest,
            response_body: GlobalpayVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: GlobalpayCaptureRequest,
            response_body: GlobalpayCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: GlobalpayRefundRequest,
            response_body: GlobalpayRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: GlobalpayRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: ClientAuthenticationToken,
            request_body: GlobalpayClientAuthRequest,
            response_body: GlobalpayClientAuthResponse,
            router_data: RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: GlobalpaySetupMandateRequest<T>,
            response_body: GlobalpaySetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: GlobalpayRepeatPaymentRequest,
            response_body: GlobalpayRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: ServerAuthenticationToken,
            request_body: GlobalpayAccessTokenRequest,
            response_body: GlobalpayAccessTokenResponse,
            router_data: RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ),
        (
            flow: PaymentMethodToken,
            request_body: GlobalpayPaymentMethodTokenRequest<T>,
            response_body: GlobalpayPaymentMethodTokenResponse,
            router_data: RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ),
        (
            flow: PostAuthenticate,
            request_body: GlobalpayConfirmRequest,
            response_body: GlobalpayConfirmResponse,
            router_data: RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMinorUnit
    ],
    member_functions: {
        pub fn build_headers(
            &self,
            access_token: &str,
        ) -> Vec<(String, Maskable<String>)> {
            vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::X_GP_VERSION.to_string(),
                    API_VERSION.to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Bearer {access_token}").into_masked(),
                ),
            ]
        }

        pub fn get_headers_from_access_token(
            &self,
            access_token: Option<ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let token = access_token.ok_or_else(|| IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Ensure the OAuth access token is obtained via the \
                         ServerAuthenticationToken flow before initiating this operation."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "GlobalPay requires an OAuth access token on \
                         `resource_common_data.access_token`, but it was None."
                            .to_string(),
                    ),
                },
            })?;
            Ok(self.build_headers(&token.access_token.expose()))
        }

        /// Get base URL for payment endpoints
        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.globalpay.base_url
        }

        /// Get base URL for refund endpoints
        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.globalpay.base_url
        }

        /// Get base URL for merchant authentication endpoints
        pub fn connector_base_url_merchant_auth<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, MerchantAuthenticationFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.globalpay.base_url
        }
    }
);

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATIONS =====
// Main service trait - aggregates all other traits

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Globalpay<T>
{
}

// ===== PAYMENT FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Globalpay<T>
{
}

// ===== REFUND FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Globalpay<T>
{
}

// ===== ADVANCED FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Globalpay<T>
{
}

// ===== WEBHOOK TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Globalpay<T>
{
    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let signature = request.headers.get("x-gp-signature").ok_or_else(|| {
            error_stack::report!(WebhookError::WebhookSignatureNotFound).attach_printable(
                "x-gp-signature header is missing from the GlobalPay webhook request",
            )
        })?;
        Ok(signature.as_bytes().to_vec())
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let secret = std::str::from_utf8(&connector_webhook_secret.secret)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("GlobalPay webhook secret bytes are not valid UTF-8")?;

        // Normalise through serde_json::Value so the key order is deterministic.
        let body_value: serde_json::Value = request
            .body
            .parse_struct("GlobalpayWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable(
                "Failed to parse GlobalPay webhook body as JSON for signature construction",
            )?;

        let mut message = serde_json::to_string(&body_value)
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable("Failed to re-serialize GlobalPay webhook body to JSON string")?;

        // GlobalPay signature = SHA-512(serialised_json + merchant_secret)
        message.push_str(secret);
        Ok(message.into_bytes())
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        let secrets = connector_webhook_secret.ok_or_else(|| {
            error_stack::report!(WebhookError::WebhookVerificationSecretNotFound).attach_printable(
                "Webhook secret is required for GlobalPay signature verification; \
                     configure it in the connector's webhook settings",
            )
        })?;

        let received_signature =
            self.get_webhook_source_verification_signature(&request, &secrets)?;
        let message = self.get_webhook_source_verification_message(&request, &secrets)?;

        let computed_hex = hex::encode(Sha512::digest(&message));

        let received_signature_str = std::str::from_utf8(&received_signature)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("GlobalPay x-gp-signature header value is not valid UTF-8")?;

        Ok(computed_hex == received_signature_str)
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"id":"TRN_probe_001","status":"CAPTURED","type":"SALE","amount":"1099"}"#
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let body: globalpay::GlobalpayWebhookBody = request
            .body
            .parse_struct("GlobalpayWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable("Failed to parse GlobalPay webhook body for event type detection")?;

        let event_type = if body.transaction_type
            == Some(globalpay::GlobalpayWebhookTransactionType::Refund)
        {
            match body.status {
                globalpay::GlobalpayWebhookStatus::Captured
                | globalpay::GlobalpayWebhookStatus::Funded => EventType::RefundSuccess,
                globalpay::GlobalpayWebhookStatus::Declined
                | globalpay::GlobalpayWebhookStatus::Failed
                | globalpay::GlobalpayWebhookStatus::Rejected
                | globalpay::GlobalpayWebhookStatus::Reversed => EventType::RefundFailure,
                _ => EventType::RefundProcessing,
            }
        } else {
            match body.status {
                globalpay::GlobalpayWebhookStatus::Captured
                | globalpay::GlobalpayWebhookStatus::Funded => EventType::PaymentIntentSuccess,
                globalpay::GlobalpayWebhookStatus::Preauthorized => {
                    EventType::PaymentIntentAuthorizationSuccess
                }
                globalpay::GlobalpayWebhookStatus::Declined
                | globalpay::GlobalpayWebhookStatus::Failed
                | globalpay::GlobalpayWebhookStatus::Rejected => EventType::PaymentIntentFailure,
                globalpay::GlobalpayWebhookStatus::Reversed => EventType::PaymentIntentCancelled,
                _ => EventType::PaymentIntentProcessing,
            }
        };

        Ok(event_type)
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let body: globalpay::GlobalpayWebhookBody = request
            .body
            .parse_struct("GlobalpayWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable(
                "Failed to parse GlobalPay webhook body for event reference extraction",
            )?;

        let reference =
            if body.transaction_type == Some(globalpay::GlobalpayWebhookTransactionType::Refund) {
                WebhookResourceReference::Refund(RefundWebhookReference {
                    connector_refund_id: Some(body.id),
                    merchant_refund_id: None,
                    connector_transaction_id: None,
                    merchant_transaction_id: body.reference,
                })
            } else {
                WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: Some(body.id),
                    merchant_transaction_id: body.reference,
                })
            };

        Ok(Some(reference))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let raw_body = String::from_utf8_lossy(&request.body).to_string();

        let body: globalpay::GlobalpayWebhookBody = request
            .body
            .parse_struct("GlobalpayWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable(
                "Failed to parse GlobalPay webhook body in process_payment_webhook",
            )?;

        let status = AttemptStatus::from(&body.status);

        Ok(WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(body.id)),
            status,
            connector_response_reference_id: body.reference.clone(),
            connector_request_reference_id: body.reference,
            mandate_reference: None,
            error_code: None,
            error_message: None,
            error_reason: None,
            raw_connector_response: Some(raw_body),
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
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let raw_body = String::from_utf8_lossy(&request.body).to_string();

        let body: globalpay::GlobalpayWebhookBody = request
            .body
            .parse_struct("GlobalpayWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable("Failed to parse GlobalPay webhook body in process_refund_webhook")?;

        let refund_status = RefundStatus::from(&body.status);

        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: Some(body.id),
            merchant_transaction_id: body.reference.clone(),
            status: refund_status,
            connector_response_reference_id: body.reference,
            error_code: None,
            error_message: None,
            raw_connector_response: Some(raw_body),
            status_code: 200,
            response_headers: None,
        })
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, error_stack::Report<WebhookError>>
    {
        let body: globalpay::GlobalpayWebhookBody = request
            .body
            .parse_struct("GlobalpayWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable("Failed to parse GlobalPay webhook body for resource object")?;
        Ok(Box::new(body))
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Globalpay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Globalpay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Globalpay<T>
{
}

// ===== VALIDATION TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Globalpay<T>
{
    fn should_do_access_token(&self, _payment_method: Option<common_enums::PaymentMethod>) -> bool {
        true
    }
}

// Authorize flow implementation using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Globalpay,
    curl_request: Json(GlobalpayPaymentsRequest<T>),
    curl_response: GlobalpayAuthorizeResponse,
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
            self.get_headers_from_access_token(req.resource_common_data.access_token.clone())
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/transactions", self.connector_base_url_payments(req)))
        }
    }
);

// Payment Sync flow implementation using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Globalpay,
    curl_response: GlobalpayPSyncResponse,
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
            self.get_headers_from_access_token(req.resource_common_data.access_token.clone())
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let transaction_id = req
                .request
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "connector_transaction_id is required to construct the \
                             GET /transactions/{id} URL for GlobalPay PSync"
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Ensure the payment was initiated and a connector_transaction_id \
                             was captured before attempting a sync"
                                .to_string(),
                        ),
                        doc_url: None,
                    },
                })?;
            Ok(format!("{}/transactions/{}", self.connector_base_url_payments(req), transaction_id))
        }
    }
);

// Payment Void flow implementation using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Globalpay,
    curl_request: Json(GlobalpayVoidRequest),
    curl_response: GlobalpayVoidResponse,
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
            self.get_headers_from_access_token(req.resource_common_data.access_token.clone())
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let transaction_id = &req.request.connector_transaction_id;
            Ok(format!("{}/transactions/{}/reversal", self.connector_base_url_payments(req), transaction_id))
        }
    }
);

// Payment Capture flow implementation using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Globalpay,
    curl_request: Json(GlobalpayCaptureRequest),
    curl_response: GlobalpayCaptureResponse,
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
            self.get_headers_from_access_token(req.resource_common_data.access_token.clone())
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let transaction_id = req
                .request
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "connector_transaction_id is required to construct the \
                             POST /transactions/{id}/capture URL for GlobalPay Capture"
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Ensure the payment was authorized and a connector_transaction_id \
                             was captured before attempting a capture"
                                .to_string(),
                        ),
                        doc_url: None,
                    },
                })?;
            Ok(format!("{}/transactions/{}/capture", self.connector_base_url_payments(req), transaction_id))
        }
    }
);

// Refund flow implementation using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Globalpay,
    curl_request: Json(GlobalpayRefundRequest),
    curl_response: GlobalpayRefundResponse,
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
            self.get_headers_from_access_token(req.resource_common_data.access_token.clone())
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let transaction_id = req.request.connector_transaction_id.clone();
            Ok(format!("{}/transactions/{}/refund", self.connector_base_url_refunds(req), transaction_id))
        }
    }
);

// Refund Sync flow implementation using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Globalpay,
    curl_response: GlobalpayRSyncResponse,
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
            self.get_headers_from_access_token(req.resource_common_data.access_token.clone())
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let refund_id = req.request.connector_refund_id.clone();
            Ok(format!("{}/transactions/{}", self.connector_base_url_refunds(req), refund_id))
        }
    }
);

// Setup Mandate flow implementation using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Globalpay,
    curl_request: Json(GlobalpaySetupMandateRequest<T>),
    curl_response: GlobalpaySetupMandateResponse,
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
            self.get_headers_from_access_token(req.resource_common_data.access_token.clone())
        }
        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // GlobalPay mandate setup tokenizes the card via /payment-methods so the
            // resulting PMT_ id can be used as payment_method.id on subsequent MIT
            // charges through the /transactions endpoint.
            Ok(format!("{}/payment-methods", self.connector_base_url_payments(req)))
        }
    }
);

// Repeat Payment (MIT) flow - implemented via macro_connector_implementation below
// Uses the same /transactions endpoint as Authorize, with payment_method.id set to
// the connector_mandate_id from the prior SetupMandate, initiator = MERCHANT and
// stored_credential = { model: RECURRING, sequence: SUBSEQUENT }.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Globalpay,
    curl_request: Json(GlobalpayRepeatPaymentRequest),
    curl_response: GlobalpayRepeatPaymentResponse,
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
            self.get_headers_from_access_token(req.resource_common_data.access_token.clone())
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/transactions", self.connector_base_url_payments(req)))
        }
    }
);

// ClientAuthenticationToken flow implementation using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Globalpay,
    curl_request: Json(GlobalpayClientAuthRequest),
    curl_response: GlobalpayClientAuthResponse,
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
            _req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::X_GP_VERSION.to_string(),
                    API_VERSION.to_string().into(),
                ),
            ])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = &req.resource_common_data.connectors.globalpay.base_url;
            Ok(format!("{base_url}/accesstoken"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [],
    connector: Globalpay,
    curl_request: Json(GlobalpayAccessTokenRequest),
    curl_response: GlobalpayAccessTokenResponse,
    flow_name: ServerAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ServerAuthenticationTokenRequestData,
    flow_response: ServerAuthenticationTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::X_GP_VERSION.to_string(),
                    API_VERSION.to_string().into(),
                ),
            ])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/accesstoken", self.connector_base_url_merchant_auth(req)))
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            let response: GlobalpayAccessTokenErrorResponse = res
                .response
                .parse_struct("GlobalpayAccessTokenErrorResponse")
                .change_context(crate::utils::response_deserialization_fail(
                    res.status_code,
                    "globalpay: access token error response did not match expected format",
                ))?;
            with_error_response_body!(event_builder, response);
            Ok(ErrorResponse {
                status_code: res.status_code,
                code: response.error_code,
                message: response.detailed_error_description,
                reason: None,
                attempt_status: None,
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Globalpay,
    curl_request: Json(GlobalpayPaymentMethodTokenRequest<T>),
    curl_response: GlobalpayPaymentMethodTokenResponse,
    flow_name: PaymentMethodToken,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentMethodTokenizationData<T>,
    flow_response: PaymentMethodTokenResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.get_headers_from_access_token(req.resource_common_data.access_token.clone())
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/payment-methods", self.connector_base_url_payments(req)))
        }
    }
);

// PostAuthenticate (Confirm Transaction) flow
//
// Triggered after the payer completes an APM redirect (e.g. PayPal). GlobalPay
// requires an explicit POST /transactions/{id}/confirmation call to transfer funds.
// The connector transaction ID (TRN_xxx) is sourced from
// `resource_common_data.connector_order_id`, which the Prism router populates from
// the `connector_order_reference_id` field of the PostAuthenticate gRPC request.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Globalpay,
    curl_request: Json(GlobalpayConfirmRequest),
    curl_response: GlobalpayConfirmResponse,
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
            self.get_headers_from_access_token(req.resource_common_data.access_token.clone())
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let connector_transaction_id = req
                .request
                .connector_order_reference_id
                .as_deref()
                .ok_or_else(|| IntegrationError::MissingRequiredField {
                    field_name: "connector_order_reference_id",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "connector_order_reference_id (TRN_xxx) is required to construct the \
                             POST /transactions/{id}/confirmation URL for GlobalPay PostAuthenticate."
                                .to_string(),
                        ),
                        suggested_action: None,
                        doc_url: None,
                    },
                })?;

            Ok(format!(
                "{}/transactions/{}/confirmation",
                self.connector_base_url_payments(req),
                connector_transaction_id
            ))
        }
    }
);

// ===== CONNECTOR COMMON IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Globalpay<T>
{
    fn id(&self) -> &'static str {
        "globalpay"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.globalpay.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: globalpay::GlobalpayErrorResponse = res
            .response
            .parse_struct("GlobalpayErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "globalpay: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error_code,
            message: response.detailed_error_description,
            reason: None,
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
    connector: Globalpay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        IncrementalAuthorization,
        VoidPC,
        MandateRevoke,
        CreateOrder,
        ServerSessionAuthenticationToken,
        Accept,
        DefendDispute,
        SubmitEvidence,
        PreAuthenticate,
        Authenticate,
        CreateConnectorCustomer,
        GetConnectorCustomer,
    ],
    not_supported: [
        VoidPostRefund,
    ],
);
