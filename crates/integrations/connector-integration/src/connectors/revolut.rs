mod transformers;
use super::macros;

use domain_types::{
    connector_flow::{Authorize, Capture, ClientAuthenticationToken, PSync, RSync, Refund, Void},
    connector_types::{
        ClientAuthenticationTokenRequestData, ConnectorWebhookSecrets, EventContext, EventType,
        PaymentFlowData, PaymentVoidData, PaymentWebhookReference, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData, RedirectDetailsResponse,
        RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse, RefundWebhookReference,
        RefundsData, RefundsResponseData, RequestDetails, ResponseId, WebhookDetailsResponse,
        WebhookResourceReference,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};

use std::fmt::Debug;

use common_enums::{AttemptStatus, CurrencyUnit};
use common_utils::{
    crypto::{self, VerifySignature},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
};

use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::{IntegrationError, WebhookError};
use error_stack::{report, ResultExt};
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types,
    decode::BodyDecoding,
    verification::{ConnectorSourceVerificationSecrets, SourceVerification},
};
use serde::Serialize;
use transformers as revolut;
use transformers::{
    RevolutCaptureRequest, RevolutClientAuthRequest, RevolutClientAuthResponse,
    RevolutOrderCreateRequest, RevolutOrderCreateResponse,
    RevolutOrderCreateResponse as RevolutPSyncResponse,
    RevolutOrderCreateResponse as RevolutCaptureResponse, RevolutRefundRequest,
    RevolutRefundResponse, RevolutRefundResponse as RevolutRSyncResponse,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

macros::macro_connector_payout_implementation!(
    connector: Revolut,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Revolut<T>
{
    fn validate_psync_reference_id(
        &self,
        data: &PaymentsSyncData,
        _payment_flow_data: &PaymentFlowData,
    ) -> CustomResult<(), IntegrationError> {
        connector_types::validate_psync_connector_transaction_id(data)
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Revolut<T>
{
    fn verify_redirect_response_source(
        &self,
        _request: &RequestDetails,
        _secrets: Option<ConnectorSourceVerificationSecrets>,
    ) -> CustomResult<bool, IntegrationError> {
        // Revolut does not support source verification for redirect responses
        Ok(false)
    }

    fn process_redirect_response(
        &self,
        _request: &RequestDetails,
        _connector_feature_data: Option<&hyperswitch_masking::Secret<String>>,
    ) -> CustomResult<RedirectDetailsResponse, IntegrationError> {
        Ok(RedirectDetailsResponse {
            resource_id: None,
            status: None,
            connector_response_reference_id: None,
            error_code: None,
            error_message: None,
            error_reason: None,
            response_money: None,
            raw_connector_response: None,
            connector_feature_data: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Revolut<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::webhooks::IncomingWebhook for Revolut<T>
{
    fn get_webhook_integrity_checks(&self) -> Vec<connector_types::WebhookIntegrityCheck> {
        vec![connector_types::WebhookIntegrityCheck::ConnectorTransactionId]
    }

    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let signature_header = request
            .headers
            .get("revolut-signature")
            .ok_or_else(|| report!(WebhookError::WebhookSignatureNotFound))
            .attach_printable("Missing incoming webhook signature for Revolut")?;

        // Revolut signature format is "v1=hex_signature".
        // We need to split by '=' and take the second part.
        let signature_parts: Vec<&str> = signature_header.split('=').collect();

        let hex_signature = signature_parts
            .get(1)
            .ok_or_else(|| report!(WebhookError::WebhookSignatureNotFound))
            .attach_printable("Invalid signature format for Revolut")?;

        hex::decode(hex_signature)
            .attach_printable("Failed to decode hex signature")
            .change_context(WebhookError::WebhookSourceVerificationFailed)
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        _connector_webhook_secrets: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        // 1. Get the Timestamp
        let timestamp = request
            .headers
            .get("revolut-request-timestamp")
            .ok_or_else(|| report!(WebhookError::WebhookBodyDecodingFailed))
            .attach_printable("Missing timestamp header for Revolut")?;

        // 2. Get the Raw Body
        let body = std::str::from_utf8(&request.body)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Webhook source verification message parsing failed for Revolut")?;

        // 3. Construct the signing string: "v1.{timestamp}.{body}"
        let message = format!("v1.{}.{}", timestamp, body);

        Ok(message.into_bytes())
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        // Revolut uses HMAC-SHA256
        let algorithm = crypto::HmacSha256;

        let connector_webhook_secrets = match connector_webhook_secret {
            Some(secrets) => secrets,
            None => {
                // If webhook secrets are not provided, take them from connector_account_details
                let auth = revolut::RevolutAuthType::try_from(
                    connector_account_details
                        .as_ref()
                        .ok_or_else(|| report!(WebhookError::WebhookVerificationSecretNotFound))?,
                )
                .map_err(|e| e.change_context(WebhookError::WebhookSourceVerificationFailed))?;

                ConnectorWebhookSecrets {
                    secret: auth
                        .signing_secret
                        .as_ref()
                        .ok_or_else(|| report!(WebhookError::WebhookVerificationSecretNotFound))?
                        .peek()
                        .as_bytes()
                        .to_vec(),
                    additional_secret: None,
                }
            }
        };

        let signature =
            self.get_webhook_source_verification_signature(&request, &connector_webhook_secrets)?;

        let message =
            self.get_webhook_source_verification_message(&request, &connector_webhook_secrets)?;

        algorithm
            .verify_signature(&connector_webhook_secrets.secret, &signature, &message)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Webhook source verification failed for Revolut")
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"event":"ORDER_COMPLETED","order_id":"probe_order_001"}"#
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let notif: revolut::RevolutWebhookBody = request
            .body
            .parse_struct("RevolutWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        if notif
            .merchant_order_ext_ref
            .as_deref()
            .is_some_and(|reference| reference.starts_with(revolut::REFUND_REFERENCE_PREFIX))
        {
            return match notif.event {
                revolut::RevolutWebhookEvent::OrderCompleted => Ok(EventType::RefundSuccess),
                revolut::RevolutWebhookEvent::OrderCancelled
                | revolut::RevolutWebhookEvent::OrderFailed
                | revolut::RevolutWebhookEvent::OrderPaymentDeclined
                | revolut::RevolutWebhookEvent::OrderPaymentFailed => Ok(EventType::RefundFailure),
                _ => Err(WebhookError::WebhookEventTypeNotFound.into()),
            };
        }
        match notif.event {
            revolut::RevolutWebhookEvent::OrderCompleted => Ok(EventType::PaymentIntentSuccess),
            revolut::RevolutWebhookEvent::OrderAuthorised => {
                Ok(EventType::PaymentIntentAuthorizationSuccess)
            }
            revolut::RevolutWebhookEvent::OrderCancelled => Ok(EventType::PaymentIntentCancelled),
            revolut::RevolutWebhookEvent::OrderFailed => Ok(EventType::PaymentIntentFailure),
            revolut::RevolutWebhookEvent::OrderPaymentAuthenticated => {
                Ok(EventType::PaymentIntentAuthorizationSuccess)
            }
            revolut::RevolutWebhookEvent::OrderPaymentDeclined => {
                Ok(EventType::PaymentIntentAuthorizationFailure)
            }
            revolut::RevolutWebhookEvent::OrderPaymentFailed => Ok(EventType::PaymentIntentFailure),
            revolut::RevolutWebhookEvent::PayoutInitiated => Ok(EventType::PayoutCreated),
            revolut::RevolutWebhookEvent::PayoutCompleted => Ok(EventType::PayoutSuccess),
            revolut::RevolutWebhookEvent::PayoutFailed => Ok(EventType::PayoutFailure),
            revolut::RevolutWebhookEvent::DisputeActionRequired => Ok(EventType::DisputeOpened),
            revolut::RevolutWebhookEvent::DisputeUnderReview => Ok(EventType::DisputeOpened),
            revolut::RevolutWebhookEvent::DisputeWon => Ok(EventType::DisputeWon),
            revolut::RevolutWebhookEvent::DisputeLost => Ok(EventType::DisputeLost),
        }
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let body: revolut::RevolutWebhookBody = request
            .body
            .parse_struct("RevolutWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        if !matches!(
            body.event,
            revolut::RevolutWebhookEvent::OrderCompleted
                | revolut::RevolutWebhookEvent::OrderAuthorised
                | revolut::RevolutWebhookEvent::OrderCancelled
                | revolut::RevolutWebhookEvent::OrderFailed
                | revolut::RevolutWebhookEvent::OrderPaymentAuthenticated
                | revolut::RevolutWebhookEvent::OrderPaymentDeclined
                | revolut::RevolutWebhookEvent::OrderPaymentFailed
        ) {
            return Ok(None);
        }
        let refund_reference = body
            .merchant_order_ext_ref
            .as_deref()
            .and_then(|reference| reference.strip_prefix(revolut::REFUND_REFERENCE_PREFIX));
        Ok(Some(if let Some(reference) = refund_reference {
            WebhookResourceReference::Refund(RefundWebhookReference {
                connector_refund_id: Some(body.order_id),
                merchant_refund_id: Some(reference.to_owned()),
                connector_transaction_id: None,
                merchant_transaction_id: None,
            })
        } else {
            WebhookResourceReference::Payment(PaymentWebhookReference {
                connector_transaction_id: Some(body.order_id),
                merchant_transaction_id: body.merchant_order_ext_ref,
                connector_preprocessing_id: None,
            })
        }))
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let body: revolut::RevolutWebhookBody = request
            .body
            .parse_struct("RevolutWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        let reference = body
            .merchant_order_ext_ref
            .as_deref()
            .and_then(|reference| reference.strip_prefix(revolut::REFUND_REFERENCE_PREFIX))
            .ok_or(WebhookError::WebhookEventTypeNotFound)?;
        let status = match body.event {
            revolut::RevolutWebhookEvent::OrderCompleted => common_enums::RefundStatus::Success,
            revolut::RevolutWebhookEvent::OrderCancelled
            | revolut::RevolutWebhookEvent::OrderFailed
            | revolut::RevolutWebhookEvent::OrderPaymentDeclined
            | revolut::RevolutWebhookEvent::OrderPaymentFailed => {
                common_enums::RefundStatus::Failure
            }
            _ => return Err(WebhookError::WebhookEventTypeNotFound.into()),
        };
        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: Some(body.order_id),
            merchant_transaction_id: None,
            connector_response_reference_id: Some(reference.to_owned()),
            status,
            error_code: None,
            error_message: None,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).into_owned()),
            status_code: 200,
            response_headers: None,
        })
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let notif: revolut::RevolutWebhookBody = request
            .body
            .parse_struct("RevolutWebhookBody")
            .attach_printable("Failed to parse Revolut webhook body")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        let response = WebhookDetailsResponse::try_from(notif)
            .change_context(WebhookError::WebhookResponseEncodingFailed);

        response.map(|mut response| {
            response.raw_connector_response =
                Some(String::from_utf8_lossy(&request.body).to_string());
            response
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Revolut<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Revolut,
    curl_request: Json(RevolutClientAuthRequest),
    curl_response: RevolutClientAuthResponse,
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
            let base_url = self.connector_base_url_merchant_auth(req);
            Ok(format!("{base_url}/api/orders"))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Revolut<T>
{
    fn should_do_order_create(&self) -> bool {
        false
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Revolut<T>
{
    fn id(&self) -> &'static str {
        "revolut"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = revolut::RevolutAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {}", auth.secret_api_key.peek()).into_masked(),
        )])
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.revolut.base_url
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: revolut::RevolutErrorResponse = res
            .response
            .parse_struct("RevolutErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "revolut: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        let (code, message, attempt_status) = match response {
            revolut::RevolutErrorResponse::StandardError { code, message, .. } => {
                let attempt_status = match code.as_str() {
                    "unauthenticated" => AttemptStatus::AuthenticationFailed,
                    "unauthorized" => AttemptStatus::AuthorizationFailed,
                    "not_found" => AttemptStatus::Failure,
                    "invalid_request" => AttemptStatus::Failure,
                    "payment_declined" => AttemptStatus::Failure,
                    _ => AttemptStatus::Pending,
                };
                (code, message, attempt_status)
            }
            revolut::RevolutErrorResponse::ErrorIdResponse { error_id, code, .. } => {
                let (error_code, attempt_status) = match code {
                    Some(numeric_code) => {
                        let status = match numeric_code {
                            1024 => AttemptStatus::Failure,
                            _ => AttemptStatus::Pending,
                        };
                        (numeric_code.to_string(), status)
                    }
                    None => ("UNKNOWN_ERROR".to_string(), AttemptStatus::Failure),
                };
                (
                    error_code,
                    format!("Error ID: {}", error_id),
                    attempt_status,
                )
            }
        };

        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message: message.clone(),
            reason: Some(message),
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

macros::create_amount_converter_wrapper!(connector_name: Revolut, amount_type: MinorUnit);

macros::create_all_prerequisites!(
    connector_name: Revolut,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: RevolutOrderCreateRequest,
            response_body: RevolutOrderCreateResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: RevolutPSyncResponse,
            response_router_data: connector,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: RevolutCaptureRequest,
            response_body: RevolutCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: RevolutRefundRequest,
            response_body: RevolutRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: RevolutRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: ClientAuthenticationToken,
            request_body: RevolutClientAuthRequest,
            response_body: RevolutClientAuthResponse,
            router_data: RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                ),
                (
                    "Revolut-Api-Version".to_string(),
                    "2024-09-01".to_string().into(),
                ),
            ];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }

        pub fn connector_base_url<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> String {
            req.resource_common_data.connectors.revolut.base_url.to_string()
        }

        pub fn connector_base_url_merchant_auth<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, MerchantAuthenticationFlowData, Req, Res>,
        ) -> String {
            req.resource_common_data.connectors.revolut.base_url.to_string()
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Revolut,
    curl_request: Json(RevolutOrderCreateRequest),
    curl_response: RevolutOrderCreateResponse,
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
            Ok(format!("{base_url}/api/orders"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Revolut,
    curl_response: RevolutOrderCreateResponse,
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
            let order_id = req.request.connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?;
            let base_url = self.connector_base_url(req);
            Ok(format!("{base_url}/api/orders/{order_id}"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Revolut,
    curl_request: Json(RevolutCaptureRequest),
    curl_response: RevolutCaptureResponse,
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
            let mut headers = self.build_headers(req)?;

            headers.push((
                "Revolut-Api-Version".to_string(),
                "2024-09-01".to_string().into(),
            ));
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let order_id = req.request.connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?;
            let base_url = self.connector_base_url(req);
            Ok(format!("{base_url}/api/orders/{order_id}/capture"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Revolut,
    curl_request: Json(RevolutRefundRequest),
    curl_response: RevolutRefundResponse,
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
            let mut headers = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                ),
                (
                    "Revolut-Api-Version".to_string(),
                    "2024-09-01".to_string().into(),
                ),
            ];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            headers.append(&mut api_key);
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let order_id = req.request.connector_transaction_id.clone();
            let base_url = req.resource_common_data.connectors.revolut.base_url.to_string();
            Ok(format!("{base_url}/api/orders/{order_id}/refund"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Revolut,
    curl_response: RevolutRSyncResponse,
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
            let mut headers = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                ),
                (
                    "Revolut-Api-Version".to_string(),
                    "2024-09-01".to_string().into(),
                ),
            ];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            headers.append(&mut api_key);
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let order_id = req.request.connector_refund_id.clone();
            let base_url = req.resource_common_data.connectors.revolut.base_url.to_string();
            Ok(format!("{base_url}/api/orders/{order_id}"))
        }
    }
);

macros::macro_connector_flow_status_impls!(
    connector: Revolut,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        IncrementalAuthorization,
        CreateOrder,
        ServerAuthenticationToken,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        DefendDispute,
        Accept,
        SubmitEvidence,
        SetupMandate,
        RepeatPayment,
        MandateRevoke,
        ServerSessionAuthenticationToken,
        CreateConnectorCustomer,
        GetConnectorCustomer,
    ],
    not_supported: [
        VoidPostRefund,
        VoidPC,
        PaymentMethodToken,
    ],
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Revolut<T>
{
}

// Cancel returns HTTP 200 without an order body. Do not run it through the
// order JSON decoder, and never interpret an HTTP error as cancellation.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
    for Revolut<T>
{
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
        let order_id = uuid::Uuid::parse_str(&req.request.connector_transaction_id)
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "connector_transaction_id",
                context: Default::default(),
            })?;
        Ok(format!(
            "{}/api/orders/{order_id}/cancel",
            self.connector_base_url(req)
        ))
    }
    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, config)
    }
    fn handle_response_v2(
        &self,
        data: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        _event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ConnectorError,
    > {
        if res.status_code != 200 {
            return Err(ConnectorError::response_handling_failed(res.status_code).into());
        }
        Ok(RouterDataV2 {
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Voided,
                ..data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    data.request.connector_transaction_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: res.status_code,
                splits: None,
                payment_account_reference: None,
            }),
            ..data.clone()
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use domain_types::{connector_types::HttpMethod, payment_method_data::DefaultPCIHolder};
    use interfaces::connector_types::IncomingWebhook;

    fn webhook(event: &str, reference: Option<&str>) -> RequestDetails {
        RequestDetails {
            method: HttpMethod::Post,
            uri: None,
            headers: Default::default(),
            query_params: None,
            body: serde_json::to_vec(&serde_json::json!({
                "event": event, "order_id": "revolut-order-id", "merchant_order_ext_ref": reference
            }))
            .unwrap(),
        }
    }

    #[test]
    fn refund_events_are_routed_and_correlated_to_the_refund_not_the_payment() {
        let connector = Revolut::<DefaultPCIHolder>::new();
        for event in [
            "ORDER_COMPLETED",
            "ORDER_FAILED",
            "ORDER_CANCELLED",
            "ORDER_PAYMENT_FAILED",
            "ORDER_PAYMENT_DECLINED",
        ] {
            let request = webhook(event, Some("prism:revolut:refund:refund-123"));
            let success = event == "ORDER_COMPLETED";
            assert_eq!(
                connector.get_event_type(request.clone()).unwrap(),
                if success {
                    EventType::RefundSuccess
                } else {
                    EventType::RefundFailure
                }
            );
            let details = connector
                .process_refund_webhook(request.clone(), None, None)
                .unwrap();
            assert_eq!(
                details.status,
                if success {
                    common_enums::RefundStatus::Success
                } else {
                    common_enums::RefundStatus::Failure
                }
            );
            assert_eq!(
                details.connector_refund_id.as_deref(),
                Some("revolut-order-id")
            );
            assert_eq!(
                details.connector_response_reference_id.as_deref(),
                Some("refund-123")
            );
            assert!(matches!(
                connector.get_webhook_event_reference(request).unwrap(),
                Some(WebhookResourceReference::Refund(_))
            ));
        }
    }

    #[test]
    fn normal_payment_events_are_not_misclassified_as_refunds() {
        let connector = Revolut::<DefaultPCIHolder>::new();
        for reference in [None, Some("payment-123"), Some("refund-123")] {
            let request = webhook("ORDER_COMPLETED", reference);
            assert_eq!(
                connector.get_event_type(request.clone()).unwrap(),
                EventType::PaymentIntentSuccess
            );
            assert!(connector
                .process_refund_webhook(request.clone(), None, None)
                .is_err());
            assert!(matches!(
                connector.get_webhook_event_reference(request).unwrap(),
                Some(WebhookResourceReference::Payment(_))
            ));
        }
    }
    fn void_data() -> RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData> {
        use domain_types::{payment_address::PaymentAddress, types::Connectors};
        use std::sync::Arc;
        RouterDataV2 {
            flow: PhantomData,
            resource_common_data: PaymentFlowData {
                merchant_id: common_utils::id_type::MerchantId::default(),
                customer_id: None,
                connector_customer: None,
                payment_id: "pay_123".to_string(),
                attempt_id: "attempt_123".to_string(),
                status: AttemptStatus::Charged,
                payment_method: common_enums::PaymentMethod::Card,
                payment_method_type: None,
                description: None,
                return_url: None,
                address: PaymentAddress::default(),
                auth_type: common_enums::AuthenticationType::NoThreeDs,
                connector_feature_data: None,
                amount_captured: None,
                amount_capturable: None,
                amount: None,
                access_token: None,
                session_token: None,
                reference_id: None,
                connector_order_id: None,
                preprocessing_id: None,
                connector_api_version: None,
                connector_request_reference_id: "ref_123".to_string(),
                test_mode: Some(true),
                connector_http_status_code: None,
                connector_response_headers: None,
                external_latency: None,
                connectors: Arc::new(Connectors::default()),
                raw_connector_response: None,
                typed_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
                vault_headers: None,
                connector_response: None,
                recurring_mandate_payment_data: None,
                order_details: None,
                amount_authorized: None,
                l2_l3_data: None,
                merchant_request_id: None,
                sender_payment_instrument_id: None,
                connector_returned_payment_method_details: None,
                settlement_status: None,
                raw_connector_status: None,
            },
            connector_config: ConnectorSpecificConfig::Revolut {
                secret_api_key: hyperswitch_masking::Secret::new("test-key".into()),
                signing_secret: None,
                base_url: None,
            },
            request: PaymentVoidData {
                connector_transaction_id: "123e4567-e89b-12d3-a456-426614174000".into(),
                payment_method_data: None,
                cancellation_reason: None,
                integrity_object: None,
                raw_connector_response: None,
                browser_info: None,
                amount: None,
                currency: None,
                connector_feature_data: None,
                metadata: None,
                merchant_order_id: None,
                split_payments: None,
            },
            response: Err(ErrorResponse::default()),
        }
    }

    #[test]
    fn cancellation_posts_to_the_existing_order_and_accepts_empty_success() {
        let connector = Revolut::<DefaultPCIHolder>::new();
        let data = void_data();
        let request = connector.build_request_v2(&data).unwrap().unwrap();
        assert_eq!(request.method, common_utils::request::Method::Post);
        assert!(request
            .url
            .ends_with("/api/orders/123e4567-e89b-12d3-a456-426614174000/cancel"));
        let result = connector
            .handle_response_v2(
                &data,
                None,
                Response {
                    status_code: 200,
                    response: bytes::Bytes::new(),
                    headers: None,
                },
            )
            .unwrap();
        assert_eq!(result.resource_common_data.status, AttemptStatus::Voided);
        match result.response.unwrap() {
            PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(id),
                ..
            } => assert_eq!(id, data.request.connector_transaction_id),
            _ => panic!("expected cancelled order ID"),
        }
        assert!(connector
            .handle_response_v2(
                &data,
                None,
                Response {
                    status_code: 422,
                    response: bytes::Bytes::new(),
                    headers: None,
                }
            )
            .is_err());
        let mut invalid = data;
        invalid.request.connector_transaction_id = "../another-order".into();
        assert!(connector.get_url(&invalid).is_err());
    }
    #[test]
    fn authorize_wire_request_reuses_customer_and_forwards_checkout_options() {
        use domain_types::{
            payment_method_data::PaymentMethodData, types::AuthorizationRequest,
            utils::ForeignTryFrom,
        };
        use hyperswitch_masking::Secret;
        use serde_json::json;
        let connector = Revolut::<DefaultPCIHolder>::new();
        let proto = grpc_api_types::payments::PaymentServiceAuthorizeRequest {
            payment_method: Some(grpc_api_types::payments::PaymentMethod {
                payment_method: Some(grpc_api_types::payments::payment_method::PaymentMethod::Card(Default::default())),
            }),
            amount: Some(grpc_api_types::payments::Money {
                minor_amount: 100, currency: grpc_api_types::payments::Currency::Gbp.into(),
            }),
            connector_feature_data: Some(Secret::new(json!({
                "customer_id": "fallback-customer", "location_id": "store-id", "expire_pending_after": "PT30M"
            }).to_string())),
            ..Default::default()
        };
        let request = PaymentsAuthorizeData::<DefaultPCIHolder>::foreign_try_from((
            AuthorizationRequest::from(proto),
            PaymentMethodData::Reward,
        ))
        .unwrap();
        let base = void_data();
        let mut data = RouterDataV2 {
            flow: PhantomData::<Authorize>,
            resource_common_data: base.resource_common_data,
            connector_config: base.connector_config,
            request,
            response: Err(ErrorResponse::default()),
        };
        data.resource_common_data.connector_customer = Some("existing-customer".into());
        let transform = |data| {
            RevolutOrderCreateRequest::try_from(RevolutRouterData {
                connector: connector.clone(),
                router_data: data,
            })
        };
        let body = serde_json::to_value(transform(data.clone()).unwrap()).unwrap();
        assert_eq!(body["customer"], json!({"id": "existing-customer"}));
        assert_eq!(body["location_id"], "store-id");
        assert_eq!(body["expire_pending_after"], "PT30M");
        data.resource_common_data.connector_customer = None;
        let body = serde_json::to_value(transform(data.clone()).unwrap()).unwrap();
        assert_eq!(body["customer"], json!({"id": "fallback-customer"}));
        data.request.connector_feature_data = None;
        let body = serde_json::to_value(transform(data.clone()).unwrap()).unwrap();
        assert!(body.get("customer").is_none());
        assert!(body.get("location_id").is_none());
        assert!(body.get("expire_pending_after").is_none());
        data.resource_common_data.connector_request_reference_id =
            "prism:revolut:refund:reserved".into();
        assert!(transform(data).is_err());
    }
}
